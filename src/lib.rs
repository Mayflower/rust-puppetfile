//! This library parses a Puppetfile

#![crate_name = "puppetfile"]
#![deny(missing_docs)]
#![feature(slicing_syntax)]
#![feature(globs)]

extern crate hyper;
extern crate serialize;
extern crate semver;

use std::error::{Error, FromError};
use std::fmt;
use std::io;

use hyper::Client;
use serialize::json;
use semver::VersionReq;

use ErrorKind::*;

mod puppetfile_parser;

#[cfg(test)]
mod test;

/// This represents a Puppetfile
#[deriving(PartialEq, Clone)]
#[experimental]
pub struct Puppetfile {
    /// The forge URL
    pub forge: String,
    /// All Modules contained in the Puppetfile
    pub modules: Vec<Module>
}

#[experimental]
impl Puppetfile {
    /// Try parsing the contents of a Puppetfile into a Puppetfile struct
    pub fn parse(contents: &str) -> Result<Puppetfile, String> {
        puppetfile_parser::parse(contents)
    }
}
impl fmt::Show for Puppetfile {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let res = write!(f, "forge '{}'\n\n", self.forge);
        self.modules.iter().fold(res, |prev_res, module| { prev_res.and(write!(f, "\n{}\n", module)) })
    }
}


/// The representation of a puppet module
#[deriving(PartialEq, Clone)]
#[experimental]
pub struct Module {
    /// Name of the module
    pub name: String,
    /// More information about the module
    pub info: Vec<ModuleInfo>
}

#[deriving(Decodable)]
struct ForgeVersionResponse {
    version: String
}

/// represents the type of error of a PuppetfileError
#[deriving(Clone, PartialEq, Show)]
pub enum ErrorKind {
    /// an HTTP error
    HttpError(hyper::HttpError),
    /// an IO error
    IoError(io::IoError),
    /// an error while parsing the version
    SemverError(semver::ParseError),
    /// an error while parsing JSON
    JsonError(json::DecoderError),
    /// an error while building the forge URL
    UrlBuilding,
}
/// represents an error while checking the version published on the forge
#[deriving(Clone, PartialEq, Show)]
pub struct PuppetfileError {
    /// type of the error
    pub kind: ErrorKind,
    /// short description
    pub desc: String,
    /// optional, more detailed description
    pub detail: Option<String>
}

impl FromError<hyper::HttpError> for PuppetfileError {
    fn from_error(err: hyper::HttpError) -> PuppetfileError {
        FromError::from_error((HttpError(err), "an HTTP error occured".to_string()))
    }
}

impl FromError<io::IoError> for PuppetfileError {
    fn from_error(err: io::IoError) -> PuppetfileError {
        FromError::from_error((IoError(err), "an IO error occured".to_string()))
    }
}

impl FromError<semver::ParseError> for PuppetfileError {
    fn from_error(err: semver::ParseError) -> PuppetfileError {
        FromError::from_error((SemverError(err), "an invalid version was given".to_string()))
    }
}

impl FromError<json::DecoderError> for PuppetfileError {
    fn from_error(err: json::DecoderError) -> PuppetfileError {
        FromError::from_error((JsonError(err), "an error occured while decoding JSON".to_string()))
    }
}

impl FromError<(ErrorKind, String)> for PuppetfileError {
    fn from_error((kind, desc): (ErrorKind, String)) -> PuppetfileError {
        PuppetfileError {
            kind: kind,
            desc: desc,
            detail: None,
        }
    }
}


impl Error for PuppetfileError {
    fn description(&self) -> &str {
        self.desc[]
    }

    fn detail(&self) -> Option<String> {
        self.detail.clone()
    }

    fn cause(&self) -> Option<&Error> {
        match self.kind {
            JsonError(ref err) => Some(err as &Error),
            HttpError(ref err) => Some(err as &Error),
            IoError(ref err) => Some(err as &Error),
            //SemverError(ref err) => Some(err as &Error),
            _ => None
        }
    }
}

#[experimental]
impl Module {
    /// The current version of the module returned from the forge API
    pub fn forge_version(&self, forge_url: &String) -> Result<semver::Version, PuppetfileError> {
        let url = try!(self.version_url(forge_url));
        let mut response = try!(Client::new().get(url[]).send());
        let response_string = try!(response.read_to_string());
        let version_struct: ForgeVersionResponse = try!(json::decode(response_string[]));
        let version = try!(semver::Version::parse(version_struct.version[]));

        Ok(version)
    }

    /// Builds the URL for the forge API for fetching the version
    pub fn version_url(&self, forge_url: &String) -> Result<String, PuppetfileError> {
        let stripped_url = match forge_url[].ends_with("/") {
            true => forge_url[..forge_url.len() - 1],
            _    => forge_url[]
        };
        let (user, mod_name) = match self.user_name_pair() {
            Some((user, mod_name)) => (user, mod_name),
            None => return Err(FromError::from_error((UrlBuilding, "Could not build url".to_string())))
        };

        Ok(format!("{}/users/{}/modules/{}/releases/find.json", stripped_url, user, mod_name))
    }

    /// Returns user and module name from 'user/mod_name'
    pub fn user_name_pair(&self) -> Option<(&str, &str)> {
        if self.name[].contains("/") {
            let mut parts = self.name[].split('/');
            Some((parts.next().unwrap(), parts.next().unwrap()))
        } else {
            None
        }
    }

    /// Returns the version if specified
    pub fn version(&self) -> Option<&VersionReq> {
        for info in self.info.iter() {
            match *info {
                ModuleInfo::Version(ref v) => return Some(v),
                ModuleInfo::Info(..) => ()
            }
        }
        None
    }
}
impl fmt::Show for Module {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let res = write!(f, "mod '{}'", self.name);
        self.info.iter().fold(res, |prev_res, mod_info| {
            match *mod_info {
                ModuleInfo::Version(..) => prev_res.and(write!(f, ", '{}'", mod_info)),
                ModuleInfo::Info(..) => prev_res.and(write!(f, ",\n  {}", mod_info)),
            }
        })
    }
}


/// Further Information on Puppet Modules
#[deriving(PartialEq, Clone)]
pub enum ModuleInfo {
    /// Version as String
    Version(VersionReq),
    /// Key Value based Information
    Info(String, String)
}
impl ModuleInfo {
    /// Returns `true` if the option is a `Version` value
    pub fn is_version(&self) -> bool {
        match *self {
            ModuleInfo::Version(..)    => true,
            ModuleInfo::Info(..) => false
        }
    }
}

impl fmt::Show for ModuleInfo {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            ModuleInfo::Version(ref v) => write!(f, "{}", v),
            ModuleInfo::Info(ref k, ref v) => write!(f, ":{} => '{}'", k, v)
        }
    }
}

